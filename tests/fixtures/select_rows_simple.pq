let
    Source = Sql.Database("server", "AdventureWorks"),
    Orders = Source{[Name="Orders"]}[Data],
    Filtered = Table.SelectRows(Orders, each [Amount] > 100),
    Result = Table.SelectColumns(Filtered, {"OrderID", "Amount"})
in
    Result
