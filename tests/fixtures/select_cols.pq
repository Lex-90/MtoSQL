let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Selected = Table.SelectColumns(Orders, {"OrderID", "CustomerID", "Amount", "OrderDate"})
in
    Selected
