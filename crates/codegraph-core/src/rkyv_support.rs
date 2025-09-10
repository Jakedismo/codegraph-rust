use crate::{node::CodeNode, types::{Language, NodeType, Location}};
use rkyv::{Archive, Deserialize as RDeserialize, Serialize as RSerialize};
use serde::{Deserialize, Serialize};
use crate::shared::SharedStr;
use bytes::Bytes;
use rkyv::rancor::{Failure, Strategy};

// A compact, rkyv-friendly representation of CodeNode for zero-copy serialization.
// Avoids chrono/uuid complexities by storing a simplified id and omitting metadata/embedding.
#[derive(Archive, RSerialize, RDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct CompactCodeNode {
    pub id_low: u64,
    pub id_high: u64,
    pub name: SharedStr,
    pub node_type: Option<NodeType>,
    pub language: Option<Language>,
    pub location: Location,
    pub content: Option<SharedStr>,
}

impl From<&CodeNode> for CompactCodeNode {
    fn from(n: &CodeNode) -> Self {
        let (id_high, id_low) = n.id.as_u128().to_be_bytes().split_at(8);
        let mut high = [0u8; 8];
        high.copy_from_slice(id_high);
        let mut low = [0u8; 8];
        low.copy_from_slice(id_low);
        Self {
            id_high: u64::from_be_bytes(high),
            id_low: u64::from_be_bytes(low),
            name: n.name.clone().into(),
            node_type: n.node_type.clone(),
            language: n.language.clone(),
            location: n.location.clone(),
            content: n.content.clone(),
        }
    }
}

pub fn archive_nodes(nodes: &[CodeNode]) -> Bytes {
    let compact: Vec<CompactCodeNode> = nodes.iter().map(CompactCodeNode::from).collect();
    serialize_to_bytes(&compact)
}

/// Access archived compact nodes from bytes without copying.
pub fn access_archived_nodes(bytes: &[u8]) -> Result<&<Vec<CompactCodeNode> as Archive>::Archived, Failure> {
    rkyv::access::<Vec<CompactCodeNode>, Strategy<(), Failure>>(bytes)
}

fn serialize_to_bytes<T>(value: &T) -> Bytes
where
    T: rkyv::Serialize<Strategy<(), Failure>>,
{
    use rkyv::ser::{serializers::AllocSerializer, Serializer};
    let mut serializer = AllocSerializer::<256>::default();
    serializer.serialize_value(value).expect("serialize");
    let buf = serializer.into_serializer().into_inner();
    Bytes::from(buf)
}
