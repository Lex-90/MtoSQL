let
    Origine              = Sql.Database(sql_server, sql_db),
    dbo_V_PBI_ANAG       = Origine{[Schema="dbo", Item="V_PBI_ANAG_AGENTI"]}[Data],
    Filtered             = Table.SelectRows(dbo_V_PBI_ANAG, each [ATTIVO] = flag_attivi),
    #"Rimosse colonne"   = Table.RemoveColumns(Filtered, {"PARTITA_IVA", "COD_SDI"})
in
    #"Rimosse colonne"
