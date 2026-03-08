let
    Source = Sql.Database("srv", "AdventureWorks"),
    RawOrders = Source{[Name="Orders"]}[Data],
    Filtered = Table.SelectRows(RawOrders, each [Amount] > 100),
    Result = Table.SelectColumns(Filtered, {"OrderID", "Amount"})
in
    Result
