let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Pivoted = Table.Pivot(Orders, List.Distinct(Orders), "Attribute", "Value")
in
    Pivoted
