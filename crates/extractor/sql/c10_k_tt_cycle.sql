-- K_TT_CYCLE -- per-truck cycle approximation from MCH_OPERATION (NOT the container
-- handling span). A truck visits a QC once per delivery, so the interval between one
-- trucks consecutive QC moves is approximately one transport cycle (pickup, drive to
-- yard, return, next pickup). Validated read-only probe 2026-06-10: median ~14m, which
-- matches the live GPS cycle ~13m. Type-A template: DAY_STR + TIME_PREDICATE on the
-- completion timestamp (MchOper). Cap 120..1200s = 2..20m, same band as the websocket
-- cycle. Load: LOW (one pass over the MCH_OPERATION day slice K_MPH already scans).
WITH base AS (
  SELECT TRK_ID,
         TO_DATE(MCH_OPER_COMPDATE||MCH_OPER_COMPTIME,'YYYYMMDDHH24MISS') AS comp_ts
    FROM TOSADM.MCH_OPERATION
   WHERE MCH_OPER_COMPDATE = '{{DAY_STR}}'
   {{TIME_PREDICATE}}
     AND TRK_ID IS NOT NULL
     AND REGEXP_LIKE(MCH_OPER_MACHNO,'^C[0-9]+$')
     AND MCH_OPER_JOBTYPE IN ('LD','DS')
     AND LENGTH(MCH_OPER_COMPTIME) = 6
     AND SUBSTR(MCH_OPER_COMPTIME,1,2) <= '23'
     AND SUBSTR(MCH_OPER_COMPTIME,3,2) <= '59'
),
seq AS (
  SELECT TRK_ID,
         (comp_ts - LAG(comp_ts) OVER (PARTITION BY TRK_ID ORDER BY comp_ts)) * 86400 AS gap_sec
    FROM base
),
capped AS (
  SELECT TRK_ID, gap_sec FROM seq WHERE gap_sec BETWEEN 120 AND 1200
)
SELECT /*+ NO_PARALLEL */
       COUNT(DISTINCT TRK_ID)                                          AS trucks,
       COUNT(*)                                                        AS samples,
       ROUND(AVG(gap_sec), 1)                                          AS avg_sec,
       ROUND(MEDIAN(gap_sec), 1)                                       AS med_sec,
       ROUND(PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY gap_sec), 1) AS p25_sec,
       ROUND(PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY gap_sec), 1) AS p75_sec
  FROM capped
