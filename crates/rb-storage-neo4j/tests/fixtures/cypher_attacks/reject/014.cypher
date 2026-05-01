MERGE (n:Node {id: $id}) ON CREATE SET n.created = timestamp(); RETURN n
