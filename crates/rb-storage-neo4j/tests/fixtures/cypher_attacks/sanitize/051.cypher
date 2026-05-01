MATCH (n:Module)-[r:IMPORTS]->(m:Module) RETURN n.name, m.name
