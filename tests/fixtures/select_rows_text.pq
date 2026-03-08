let
    Source = Sql.Database("srv", "db"),
    Products = Source{[Name="Products"]}[Data],
    StartsWithA = Table.SelectRows(Products, each Text.StartsWith([Name], "A")),
    EndsWithZ = Table.SelectRows(StartsWithA, each Text.EndsWith([Category], "z")),
    ContainsX = Table.SelectRows(EndsWithZ, each Text.Contains([Description], "special"))
in
    ContainsX
