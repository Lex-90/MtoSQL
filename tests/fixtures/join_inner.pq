let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Customers = Source{[Name="Customers"]}[Data],
    Joined = Table.Join(Orders, "CustomerID", Customers, "ID", JoinKind.Inner)
in
    Joined
