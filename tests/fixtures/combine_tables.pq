let
    Source  = Sql.Database("srv", "db"),
    Orders  = Source{[Name="Orders"]}[Data],
    Returns = Source{[Name="Returns"]}[Data],
    All     = Table.Combine({Orders, Returns})
in
    All
