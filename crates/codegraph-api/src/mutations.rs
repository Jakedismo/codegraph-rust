
use async_graphql::{Context, Object, Result, InputObject, ID, Union, OneOfObject};
use codegraph_graph::TransactionalGraph;
use codegraph_core::{CodeNode, NodeType, Location, Language, NodeId};

#[derive(InputObject)]
pub struct AddNodeInput {
    pub name: String,
    pub node_type: NodeType,
    pub language: Language,
    pub file_path: String,
    pub start_line: i32,
    pub start_column: i32,
    pub end_line: i32,
    pub end_column: i32,
}

#[derive(InputObject)]
pub struct UpdateNodeInput {
    pub id: ID,
    pub name: Option<String>,
    pub node_type: Option<NodeType>,
    pub language: Option<Language>,
    pub file_path: Option<String>,
    pub start_line: Option<i32>,
    pub start_column: Option<i32>,
    pub end_line: Option<i32>,
    pub end_column: Option<i32>,
}

#[derive(InputObject)]
pub struct DeleteNodeInput {
    pub id: ID,
}

#[derive(OneOfObject)]
pub enum BatchOperationInput {
    AddNode(AddNodeInput),
    UpdateNode(UpdateNodeInput),
    DeleteNode(DeleteNodeInput),
}

#[derive(InputObject)]
pub struct UpdateConfigurationInput {
    pub key: String,
    pub value: String,
}

pub struct Mutation { pub(crate) graph: TransactionalGraph }

#[Object]
impl Mutation {
    /// Adds a new node to the graph.
    async fn add_node(&self, ctx: &Context<'_>, input: AddNodeInput) -> Result<bool> {
        let mut graph = self.graph.clone();

        let node = CodeNode::new(
            input.name,
            Some(input.node_type),
            Some(input.language),
            Location {
                file_path: input.file_path,
                line: input.start_line as u32,
                column: input.start_column as u32,
                end_line: Some(input.end_line as u32),
                end_column: Some(input.end_column as u32),
            },
        );

        graph.add_node(node).await?;

        Ok(true)
    }

    /// Updates an existing node in the graph.
    async fn update_node(&self, ctx: &Context<'_>, input: UpdateNodeInput) -> Result<bool> {
        let mut graph = self.graph.clone();
        let node_id = NodeId::from(input.id.to_string());

        let mut node = graph.get_node(node_id).await?.ok_or_else(|| "Node not found")?;

        if let Some(name) = input.name {
            node.name = name;
        }
        if let Some(node_type) = input.node_type {
            node.node_type = Some(node_type);
        }
        if let Some(language) = input.language {
            node.language = Some(language);
        }
        if let Some(file_path) = input.file_path {
            node.location.file_path = file_path;
        }
        if let Some(start_line) = input.start_line {
            node.location.line = start_line as u32;
        }
        if let Some(start_column) = input.start_column {
            node.location.column = start_column as u32;
        }
        if let Some(end_line) = input.end_line {
            node.location.end_line = Some(end_line as u32);
        }
        if let Some(end_column) = input.end_column {
            node.location.end_column = Some(end_column as u32);
        }

        graph.update_node(node).await?;

        Ok(true)
    }

    /// Deletes a node from the graph.
    async fn delete_node(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let mut graph = self.graph.clone();
        let node_id = NodeId::from(id.to_string());

        graph.remove_node(node_id).await?;

        Ok(true)
    }

    /// Triggers a re-indexing operation.
    async fn trigger_reindexing(&self, ctx: &Context<'_>) -> Result<bool> {
        // Placeholder for re-indexing logic
        Ok(true)
    }

    /// Updates the configuration.
    async fn update_configuration(&self, ctx: &Context<'_>, input: UpdateConfigurationInput) -> Result<bool> {
        // Placeholder for configuration update logic
        Ok(true)
    }

    /// Executes a batch of operations in a single transaction.
    async fn batch_operations(&self, ctx: &Context<'_>, operations: Vec<BatchOperationInput>) -> Result<bool> {
        let mut graph = self.graph.clone();
        graph.begin_transaction().await?;

        for op in operations {
            match op {
                BatchOperationInput::AddNode(input) => {
                    let node = CodeNode::new(
                        input.name,
                        Some(input.node_type),
                        Some(input.language),
                        Location {
                            file_path: input.file_path,
                            line: input.start_line as u32,
                            column: input.start_column as u32,
                            end_line: Some(input.end_line as u32),
                            end_column: Some(input.end_column as u32),
                        },
                    );
                    graph.add_node(node).await?;
                }
                BatchOperationInput::UpdateNode(input) => {
                    let node_id = NodeId::from(input.id.to_string());
                    let mut node = graph.get_node(node_id).await?.ok_or_else(|| "Node not found")?;

                    if let Some(name) = input.name {
                        node.name = name;
                    }
                    if let Some(node_type) = input.node_type {
                        node.node_type = Some(node_type);
                    }
                    if let Some(language) = input.language {
                        node.language = Some(language);
                    }
                    if let Some(file_path) = input.file_path {
                        node.location.file_path = file_path;
                    }
                    if let Some(start_line) = input.start_line {
                        node.location.line = start_line as u32;
                    }
                    if let Some(start_column) = input.start_column {
                        node.location.column = start_column as u32;
                    }
                    if let Some(end_line) = input.end_line {
                        node.location.end_line = Some(end_line as u32);
                    }
                    if let Some(end_column) = input.end_column {
                        node.location.end_column = Some(end_column as u32);
                    }

                    graph.update_node(node).await?;
                }
                BatchOperationInput::DeleteNode(input) => {
                    let node_id = NodeId::from(input.id.to_string());
                    graph.remove_node(node_id).await?;
                }
            }
        }

        graph.commit().await?;

        Ok(true)
    }
}
