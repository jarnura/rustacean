MATCH (n:File)-[:DEFINES]->(s:Symbol {name: $name}) RETURN n, s
