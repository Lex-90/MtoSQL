let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Renamed = Table.RenameColumns(Orders, {{"CustomerNo", "CustomerID"}, {"Amt", "Amount"}})
in
    Renamed
