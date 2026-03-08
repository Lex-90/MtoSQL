let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Typed = Table.TransformColumnTypes(Orders, {{"OrderID", type integer}, {"Amount", type number}, {"OrderDate", type date}, {"IsActive", type logical}, {"Notes", type text}})
in
    Typed
