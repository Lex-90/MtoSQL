let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    WithTotal = Table.AddColumn(Orders, "Total", each [Quantity] * [UnitPrice], type number)
in
    WithTotal
