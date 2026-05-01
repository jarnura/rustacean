MATCH (n) RETURN count(n), collect(n.name) AS names
