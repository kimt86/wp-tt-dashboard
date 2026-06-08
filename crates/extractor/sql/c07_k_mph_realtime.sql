-- K_MPH realtime per-QC (MCH_OPERATION). Source: tos-db-research phase_c/07.
-- moves_per_active_hour = COUNT(*) / COUNT(DISTINCT hour-of-day), LD/DS only, QC only.
-- Only change: MCH_OPER_COMPDATE = '{{DAY_STR}}' (index-safe). Load: LOW.

SELECT /*+ NO_PARALLEL */
       MCH_OPER_VESSEL                                              AS vessel,
       MCH_OPER_VOYAGE                                              AS voyage,
       MCH_OPER_MACHNO                                              AS qc_machno,
       COUNT(*)                                                     AS moves,
       SUM(CASE WHEN MCH_OPER_JOBTYPE = 'LD' THEN 1 END)            AS load_moves,
       SUM(CASE WHEN MCH_OPER_JOBTYPE = 'DS' THEN 1 END)            AS discharge_moves,
       COUNT(DISTINCT SUBSTR(MCH_OPER_COMPTIME, 1, 2))              AS active_hours,
       ROUND(COUNT(*) / NULLIF(COUNT(DISTINCT SUBSTR(MCH_OPER_COMPTIME, 1, 2)), 0), 2) AS k_mph_per_active_hour,
       COUNT(DISTINCT TRK_ID)                                       AS distinct_trucks,
       COUNT(DISTINCT MCH_OPER_CONTNO)                              AS distinct_containers,
       MIN(MCH_OPER_COMPDATE || MCH_OPER_COMPTIME)                  AS first_move,
       MAX(MCH_OPER_COMPDATE || MCH_OPER_COMPTIME)                  AS last_move
  FROM TOSADM.MCH_OPERATION
 WHERE MCH_OPER_COMPDATE = '{{DAY_STR}}'
   {{TIME_PREDICATE}}
   AND REGEXP_LIKE(MCH_OPER_MACHNO, '^C[0-9]+$')
   AND MCH_OPER_JOBTYPE IN ('LD', 'DS')
 GROUP BY MCH_OPER_VESSEL, MCH_OPER_VOYAGE, MCH_OPER_MACHNO
 ORDER BY moves DESC
 FETCH FIRST 30 ROWS ONLY
