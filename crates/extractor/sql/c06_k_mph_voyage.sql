-- K_MPH official voyage-level (VSS_STATISTICS). Source: tos-db-research phase_c/06.
-- Only change: window lower bound uses the literal {{START_TS}} (= as-of - 30d, 00:00:00)
-- instead of TO_CHAR(SYSDATE-30,...). Small-table FTS. TRIVIAL.

SELECT /*+ NO_PARALLEL */
       VSS_STT_VESSEL                                              AS vessel,
       VSS_STT_VOYAGE                                              AS voyage,
       VSS_STT_UP_DT                                               AS confirmed_at,
       VSS_STT_CONFIRM                                             AS confirmed,
       VSS_STT_STTCHK                                              AS stt_check,
       TO_NUMBER(VSS_STT_VAN DEFAULT NULL ON CONVERSION ERROR)     AS containers,
       TO_NUMBER(VSS_STT_TEU DEFAULT NULL ON CONVERSION ERROR)     AS teu,
       TO_NUMBER(VSS_STT_MOVES DEFAULT NULL ON CONVERSION ERROR)   AS moves,
       TO_NUMBER(VSS_STT_SIN_MOV DEFAULT NULL ON CONVERSION ERROR) AS single_moves,
       TO_NUMBER(VSS_STT_TWN_MOV DEFAULT NULL ON CONVERSION ERROR) AS twin_moves,
       TO_NUMBER(VSS_STT_TND_MOV DEFAULT NULL ON CONVERSION ERROR) AS tandem_moves,
       TO_NUMBER(VSS_STT_GROSSTIME DEFAULT NULL ON CONVERSION ERROR) AS gross_min,
       TO_NUMBER(VSS_STT_NETTIME DEFAULT NULL ON CONVERSION ERROR)   AS net_min,
       TO_NUMBER(VSS_STT_ABERTHTIME DEFAULT NULL ON CONVERSION ERROR) AS berth_min,
       TO_NUMBER(VSS_STT_WORKQC DEFAULT NULL ON CONVERSION ERROR)  AS work_qc,
       TO_NUMBER(VSS_STT_GQCR DEFAULT NULL ON CONVERSION ERROR)    AS k_mph_gross,
       TO_NUMBER(VSS_STT_NQCR DEFAULT NULL ON CONVERSION ERROR)    AS k_mph_net,
       TO_NUMBER(VSS_STT_GBP DEFAULT NULL ON CONVERSION ERROR)     AS k_bp_gross,
       TO_NUMBER(VSS_STT_NBP DEFAULT NULL ON CONVERSION ERROR)     AS k_bp_net
  FROM TOSADM.VSS_STATISTICS
 WHERE VSS_STT_UP_DT >= '{{START_TS}}'
 ORDER BY VSS_STT_UP_DT DESC NULLS LAST
 FETCH FIRST 30 ROWS ONLY
