MATCH (n:Repo {id: $repo_id})-[:HAS_FILE]->(f:File) RETURN f
