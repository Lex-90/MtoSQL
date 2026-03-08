let
    Source = Sql.Database("srv", "db"),
    Sales = Source{[Name="Sales"]}[Data],
    Products = Source{[Name="Products"]}[Data],
    Joined = Table.Join(Sales, {"ProductID", "RegionID"}, Products, {"ID", "Region"}, JoinKind.Inner)
in
    Joined
