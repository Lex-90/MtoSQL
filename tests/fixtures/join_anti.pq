let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Customers = Source{[Name="Customers"]}[Data],
    Orphans = Table.Join(Orders, "CustomerID", Customers, "ID", JoinKind.LeftAnti)
in
    Orphans
