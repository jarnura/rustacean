use thiserror::Error;

#[derive(Debug, Error)]
pub enum CypherError {
    /// Query contains a bare semicolon outside a string/comment — multi-statement injection attempt.
    #[error("multi-statement Cypher is not permitted")]
    MultiStatement,

    /// A `(` in a path-clause context was never closed.
    #[error("unclosed node pattern: missing ')'")]
    UnclosedNodePattern,

    /// Neo4j driver error (wraps `neo4rs::Error`).
    #[error("neo4j error: {0}")]
    Neo4j(#[from] neo4rs::Error),
}
