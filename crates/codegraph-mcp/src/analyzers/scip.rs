// ABOUTME: Imports compiler-produced SCIP identities without starting one language server per run.
// ABOUTME: Streams documents in two passes and validates source identity and position encoding.
use anyhow::{Result, anyhow, bail};
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, NodeId, Span};
use codegraph_parser::SourceSnapshots;
use scip::types::{Document, Occurrence};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

fn documents(path: &Path, mut visit: impl FnMut(Document) -> Result<()>) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    if file.metadata()?.len() > 512 * 1024 * 1024 {
        bail!("SCIP artifact exceeds 512 MiB import limit");
    }
    let mut input = protobuf::CodedInputStream::new(&mut file);
    while let Some(tag) = input.read_raw_tag_or_eof()? {
        if tag == 18 {
            visit(input.read_message()?)?;
        } else {
            input.skip_field(
                protobuf::rt::WireType::new(tag & 7)
                    .ok_or_else(|| anyhow!("Invalid SCIP wire type"))?,
            )?;
        }
    }
    Ok(())
}

fn symbol_key(document: &str, symbol: &str) -> String {
    if symbol.starts_with("local ") {
        format!("{document}:{symbol}")
    } else {
        symbol.to_owned()
    }
}
fn range(occurrence: &Occurrence) -> Result<[i32; 4]> {
    use scip::types::occurrence::Typed_range;
    let range = match &occurrence.typed_range {
        Some(Typed_range::SingleLineRange(range)) => [
            range.line,
            range.start_character,
            range.line,
            range.end_character,
        ],
        Some(Typed_range::MultiLineRange(range)) => [
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        ],
        Some(_) => bail!("Unsupported SCIP range variant"),
        None => match occurrence.range.as_slice() {
            [line, start, end] => [*line, *start, *line, *end],
            [line, start, end_line, end] => [*line, *start, *end_line, *end],
            _ => bail!("Invalid SCIP occurrence range"),
        },
    };
    if range.iter().any(|value| *value < 0) || (range[0], range[1]) > (range[2], range[3]) {
        bail!("Invalid SCIP source coordinates");
    }
    Ok(range)
}
fn byte_position(
    source: &str,
    starts: &[usize],
    line: i32,
    column: i32,
    encoding: i32,
) -> Result<usize> {
    let start = *starts
        .get(line as usize)
        .ok_or_else(|| anyhow!("SCIP line outside source"))?;
    let end = starts
        .get(line as usize + 1)
        .copied()
        .unwrap_or(source.len());
    let text = &source[start..end];
    let column = column as usize;
    if encoding == 1 {
        if column <= text.len() && text.is_char_boundary(column) {
            return Ok(start + column);
        }
    } else if encoding == 2 || encoding == 3 {
        let mut units = 0;
        for (offset, character) in text.char_indices() {
            if units == column {
                return Ok(start + offset);
            }
            units += if encoding == 2 {
                character.len_utf16()
            } else {
                1
            };
        }
        if units == column {
            return Ok(end);
        }
    }
    bail!("SCIP column is invalid or document position encoding is unspecified")
}
fn source<'a>(
    root: &Path,
    document: &Document,
    sources: &'a SourceSnapshots,
    manifest: &BTreeMap<String, String>,
) -> Result<Option<&'a codegraph_parser::SourceSnapshot>> {
    let path = Path::new(&document.relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        bail!("SCIP document path escapes the project");
    }
    let Some(source) = sources.get(root.join(path)) else {
        return Ok(None);
    };
    if !document.text.is_empty() {
        if document.text.as_str() != source.contents()?.as_ref() {
            bail!("SCIP source is stale: {}", document.relative_path);
        }
    } else if manifest.get(&document.relative_path) != Some(&source.content_hash)
        && std::env::var("CODEGRAPH_SCIP_TRUST_SOURCE").as_deref() != Ok("1")
    {
        bail!(
            "SCIP source hash missing or stale for {}; supply index.sources.json captured with the compiler index, or explicitly set CODEGRAPH_SCIP_TRUST_SOURCE=1",
            document.relative_path
        );
    }
    Ok(Some(source))
}
fn containing(nodes: &[CodeNode], indices: &[usize], byte: usize) -> Option<usize> {
    let mut candidates: Vec<_> = indices
        .iter()
        .copied()
        .filter(|index| {
            nodes[*index].span.as_ref().is_some_and(|span| {
                span.start_byte as usize <= byte && byte < span.end_byte as usize
            })
        })
        .collect();
    candidates.sort_by_key(|index| {
        let span = nodes[*index].span.as_ref().unwrap();
        span.end_byte - span.start_byte
    });
    match candidates.as_slice() {
        [first] => Some(*first),
        [first, second, ..]
            if nodes[*first].span.as_ref().unwrap().end_byte
                - nodes[*first].span.as_ref().unwrap().start_byte
                != nodes[*second].span.as_ref().unwrap().end_byte
                    - nodes[*second].span.as_ref().unwrap().start_byte =>
        {
            Some(*first)
        }
        _ => None,
    }
}

pub fn import(
    path: &Path,
    root: &Path,
    sources: &SourceSnapshots,
    nodes: &mut [CodeNode],
    edges: &mut Vec<EdgeRelationship>,
    references: bool,
) -> Result<usize> {
    let manifest: BTreeMap<String, String> =
        match std::fs::read(path.with_extension("sources.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.into()),
        };
    let mut by_file = BTreeMap::<String, Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        by_file
            .entry(node.location.file_path.clone())
            .or_default()
            .push(index);
    }
    let mut definitions = BTreeMap::<String, BTreeSet<NodeId>>::new();
    documents(path, |document| {
        let Some(snapshot) = source(root, &document, sources, &manifest)? else {
            return Ok(());
        };
        let text = snapshot.contents()?;
        let starts: Vec<_> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        let Some(indices) = by_file.get(&snapshot.path.to_string_lossy().into_owned()) else {
            return Ok(());
        };
        let docs: BTreeMap<_, _> = document
            .symbols
            .iter()
            .map(|info| (info.symbol.as_str(), &info.documentation))
            .collect();
        for occurrence in &document.occurrences {
            if occurrence.symbol.is_empty() || occurrence.symbol_roles & 1 == 0 {
                continue;
            }
            let range = range(occurrence)?;
            let byte = byte_position(
                &text,
                &starts,
                range[0],
                range[1],
                document.position_encoding.value(),
            )?;
            let end = byte_position(
                &text,
                &starts,
                range[2],
                range[3],
                document.position_encoding.value(),
            )?;
            let name = text[byte..end].trim_start_matches("r#");
            let matching: Vec<_> = indices
                .iter()
                .copied()
                .filter(|index| nodes[*index].name.as_str().rsplit("::").next() == Some(name))
                .collect();
            if let Some(index) = containing(nodes, &matching, byte) {
                definitions
                    .entry(symbol_key(&document.relative_path, &occurrence.symbol))
                    .or_default()
                    .insert(nodes[index].id);
                nodes[index]
                    .metadata
                    .attributes
                    .insert("scip_symbol".into(), occurrence.symbol.clone());
                if let Some(documentation) = docs.get(occurrence.symbol.as_str()) {
                    if !documentation.is_empty() {
                        nodes[index]
                            .metadata
                            .attributes
                            .insert("doc".into(), documentation.join("\n\n"));
                    }
                }
            }
        }
        Ok(())
    })?;
    let mut resolved = 0;
    documents(path, |document| {
        let Some(snapshot) = source(root, &document, sources, &manifest)? else {
            return Ok(());
        };
        let text = snapshot.contents()?;
        let starts: Vec<_> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        let Some(indices) = by_file.get(&snapshot.path.to_string_lossy().into_owned()) else {
            return Ok(());
        };
        for occurrence in &document.occurrences {
            if occurrence.symbol.is_empty() || occurrence.symbol_roles & 1 != 0 {
                continue;
            }
            let Some(targets) =
                definitions.get(&symbol_key(&document.relative_path, &occurrence.symbol))
            else {
                continue;
            };
            if targets.len() != 1 {
                continue;
            }
            let target = *targets.first().unwrap();
            let range = range(occurrence)?;
            let start = byte_position(
                &text,
                &starts,
                range[0],
                range[1],
                document.position_encoding.value(),
            )?;
            let end = byte_position(
                &text,
                &starts,
                range[2],
                range[3],
                document.position_encoding.value(),
            )?;
            let Some(caller) = containing(nodes, indices, start) else {
                continue;
            };
            let mut matched = false;
            for edge in edges.iter_mut().filter(|edge| {
                edge.from == nodes[caller].id
                    && edge
                        .to
                        .rsplit([':', '.'])
                        .next()
                        .map(|name| name.trim_start_matches("r#"))
                        == Some(text[start..end].trim_start_matches("r#"))
                    && edge.span.as_ref().is_some_and(|span| {
                        span.start_byte as usize == start && end <= span.end_byte as usize
                    })
            }) {
                edge.metadata
                    .insert("target_node_id".into(), target.to_string());
                edge.metadata.insert("analyzer".into(), "scip".into());
                edge.metadata
                    .insert("analyzer_confidence".into(), "1.0".into());
                matched = true;
                resolved += 1;
            }
            if !matched && references {
                edges.push(EdgeRelationship {
                    from: nodes[caller].id,
                    to: occurrence.symbol.clone(),
                    edge_type: EdgeType::References,
                    metadata: std::collections::HashMap::from([
                        ("target_node_id".into(), target.to_string()),
                        ("analyzer".into(), "scip".into()),
                        (
                            "source_file".into(),
                            snapshot.path.to_string_lossy().into_owned(),
                        ),
                    ]),
                    span: Some(Span {
                        start_byte: start as u32,
                        end_byte: end as u32,
                    }),
                });
                resolved += 1;
            }
        }
        Ok(())
    })?;
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use protobuf::Message;

    #[test]
    fn unicode_positions_reject_split_surrogates_and_unspecified_encoding() {
        let text = "🚀 café\n";
        assert_eq!(byte_position(text, &[0, text.len()], 0, 3, 2).unwrap(), 5);
        assert!(byte_position(text, &[0], 0, 1, 2).is_err());
        assert!(byte_position(text, &[0], 0, 1, 1).is_err());
        assert!(byte_position(text, &[0], 0, 0, 0).is_err());
        assert_ne!(symbol_key("a.rs", "local 0"), symbol_key("b.rs", "local 0"));
    }

    #[tokio::test]
    async fn compiler_definitions_bind_calls_and_stale_artifacts_fail() -> Result<()> {
        use codegraph_core::{Language, Location, NodeType};
        let root = tempfile::tempdir()?;
        let path = root.path().join("a.rs");
        let text = "fn target() {}\nfn caller() { target(); }\n";
        std::fs::write(&path, text)?;
        let sources =
            SourceSnapshots::capture(&[(path.clone(), text.len() as u64)], 1024, 1).await?;
        let make_node = |name: &str, line, start, end| {
            let mut node = CodeNode::new(
                name,
                Some(NodeType::Function),
                Some(Language::Rust),
                Location {
                    file_path: path.to_string_lossy().into_owned(),
                    line,
                    column: 0,
                    end_line: Some(line),
                    end_column: None,
                },
            );
            node.span = Some(Span {
                start_byte: start,
                end_byte: end,
            });
            node
        };
        let mut nodes = vec![
            make_node("target", 1, 0, 14),
            make_node("caller", 2, 15, text.len() as u32),
        ];
        let mut edges = vec![EdgeRelationship {
            from: nodes[1].id,
            to: "target".into(),
            edge_type: EdgeType::Calls,
            metadata: Default::default(),
            span: Some(Span {
                start_byte: 29,
                end_byte: 37,
            }),
        }];
        let document = Document {
            relative_path: "a.rs".into(),
            text: text.into(),
            position_encoding: scip::types::PositionEncoding::UTF8CodeUnitOffsetFromLineStart
                .into(),
            occurrences: vec![
                Occurrence {
                    symbol: "compiler target".into(),
                    symbol_roles: 1,
                    range: vec![0, 3, 9],
                    ..Default::default()
                },
                Occurrence {
                    symbol: "compiler target".into(),
                    range: vec![1, 14, 20],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let artifact = root.path().join("index.scip");
        let mut index = scip::types::Index {
            documents: vec![document],
            ..Default::default()
        };
        std::fs::write(&artifact, index.write_to_bytes()?)?;
        assert_eq!(
            import(
                &artifact,
                root.path(),
                &sources,
                &mut nodes,
                &mut edges,
                false
            )?,
            1
        );
        assert_eq!(edges[0].metadata["target_node_id"], nodes[0].id.to_string());
        index.documents[0].text = "old source".into();
        std::fs::write(&artifact, index.write_to_bytes()?)?;
        assert!(
            import(
                &artifact,
                root.path(),
                &sources,
                &mut nodes,
                &mut edges,
                false
            )
            .is_err()
        );
        Ok(())
    }
}
