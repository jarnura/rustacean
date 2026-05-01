MERGE (n:Tenant {id: $id}) ON CREATE SET n.createdAt = $ts
