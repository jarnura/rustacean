MATCH (n:Symbol)-[:CALLS*1..10]->(m:Symbol) WHERE n.id = $id RETURN m
