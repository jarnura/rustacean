MATCH (n) WHERE n.name = $name RETURN n UNION MATCH (m:Alias {name: $name}) RETURN m
