let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Grouped = Table.Group(Orders, {"CustomerID"}, {{"TotalAmount", each List.Sum([Amount]), type number}, {"OrderCount", each Table.RowCount(_), type number}})
in
    Grouped
