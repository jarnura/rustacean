MATCH (n:User {id: $id}) RETURN n; CALL apoc.cypher.doIt('MATCH (m) DELETE m')
