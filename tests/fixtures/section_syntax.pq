section MyQueries;
shared Orders = let
    Source = Sql.Database("srv", "db"),
    Data = Source{[Name="Orders"]}[Data]
in Data;

shared Customers = let
    Source = Sql.Database("srv", "db"),
    Data = Source{[Name="Customers"]}[Data]
in Data;
