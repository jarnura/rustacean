MERGE (n:Entity {id: $id}) ON CREATE SET n.name = $name ON MATCH SET n.updated = $ts
