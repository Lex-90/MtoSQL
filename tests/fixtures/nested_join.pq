let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Customers = Source{[Name="Customers"]}[Data],
    Joined = Table.NestedJoin(Orders, "CustomerID", Customers, "ID", "CustData", JoinKind.Left),
    Expanded = Table.ExpandTableColumn(Joined, "CustData", {"Name", "Email"}, {"CustName", "CustEmail"})
in
    Expanded
