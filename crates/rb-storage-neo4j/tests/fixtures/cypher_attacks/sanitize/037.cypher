MERGE (a:Node {id: $a}) MERGE (b:Node {id: $b}) CREATE (a)-[:LINK]->(b)
