let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Filtered = Table.SelectRows(Orders, each List.Contains({"Active", "Pending"}, [Status]))
in
    Filtered
