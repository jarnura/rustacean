MATCH (n:User {id: $id}) SET n.lastSeen = $ts RETURN n
