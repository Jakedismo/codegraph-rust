pub mod handlers;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::post_index,
        handlers::get_search,
        handlers::get_node,
        handlers::get_neighbors,
    ),
    components(
        schemas(
            handlers::IndexRequest,
            handlers::IndexResponse,
            handlers::SearchRequest,
            handlers::SearchItem,
            handlers::SearchResponse,
            handlers::NodeResponse,
            handlers::NeighborsRequest,
            handlers::NeighborItem,
            handlers::NeighborsResponse,
            handlers::LocationDto,
        )
    ),
    tags(
        (name = "v1", description = "CodeGraph REST API v1")
    )
)]
pub struct ApiDoc;

