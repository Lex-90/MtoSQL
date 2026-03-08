let
    Source = Sql.Database("srv", "db"),
    Sales = Source{[Name="Sales"]}[Data],
    Grouped = Table.Group(Sales, {"Region", "Category"}, {{"TotalSales", each List.Sum([Amount]), type number}, {"AvgPrice", each List.Average([Price]), type number}, {"UniqueProducts", each List.CountDistinct([ProductID]), type number}})
in
    Grouped
