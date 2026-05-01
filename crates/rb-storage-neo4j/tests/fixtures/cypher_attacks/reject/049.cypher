MERGE (n:Foo {x: $x}) ON MATCH SET n.y = $y; RETURN n
