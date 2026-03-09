let
    Source   = Sql.Database("srv", "db"),
    Raw      = Source{[Name="Sales"]}[Data],
    Typed    = Table.TransformColumnTypes(Raw, {{"Amount", type number}, {"Qty", Int64.Type}}),
    SafeCols = Table.ReplaceErrorValues(Typed, {{"Amount", 0}, {"Qty", 0}})
in
    SafeCols
