MATCH (n) WHERE n.name = $name RETURN n; MATCH (m) WHERE m.name = 'admin' RETURN m.secret
