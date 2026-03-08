let
    Source = Sql.Database("srv", "AdventureWorks"),
    Orders = Source{[Name="Orders"]}[Data],
    Cleaned = Table.RemoveColumns(Orders, {"InternalCode", "AuditTimestamp"})
in
    Cleaned
