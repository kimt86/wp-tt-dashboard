-- K_UTIL TT — interval-merged (handles missing operator logout).
-- Source: tos-db-research/sql/phase_e/e3a_k_util_tt_merged.sql, validated Phase E.
-- ONLY change vs the validated original: the params CTE date source is the literal
-- {{DAY_STR}} (YYYYMMDD) injected by the extractor, instead of SYSDATE-1. All
-- downstream logic is byte-for-byte identical; the index-relevant predicate
-- (MCH_WORK_STARTDATE = p.day_str) is unchanged, so the index path is preserved.
-- Load: ~10K rows. LOW.

WITH params AS (
  SELECT TO_DATE('{{START_TS}}','YYYYMMDDHH24MISS') AS win_start,
         TO_DATE('{{END_TS}}','YYYYMMDDHH24MISS')   AS win_end,
         '{{DAY_STR}}'                              AS day_str
    FROM DUAL
),
tt_codes AS (
  SELECT CDY_MCHN_CODE FROM TOSADM.CDY_MACHINE WHERE CDY_MCHN_TYPE = 'YT'
),
sessions AS (
  SELECT mw.MCH_WORK_MACHNO AS machno,
         GREATEST(TO_DATE(SUBSTR(mw.MCH_WORK_START_DT,1,14),'YYYYMMDDHH24MISS'),
                  (SELECT win_start FROM params))    AS start_dt,
         LEAST(TO_DATE(SUBSTR(mw.MCH_WORK_END_DT,1,14),'YYYYMMDDHH24MISS'),
               (SELECT win_end FROM params))         AS end_dt
    FROM TOSADM.MCH_WORKTIME mw
    CROSS JOIN params p
   WHERE mw.MCH_WORK_MACHNO IN (SELECT CDY_MCHN_CODE FROM tt_codes)
     AND (mw.MCH_WORK_STARTDATE = p.day_str OR mw.MCH_WORK_ENDDATE = p.day_str)
     AND LENGTH(mw.MCH_WORK_START_DT) >= 14
     AND LENGTH(mw.MCH_WORK_END_DT) >= 14
),
valid AS (
  SELECT machno, start_dt, end_dt
    FROM sessions
   WHERE start_dt < end_dt
),
flagged AS (
  SELECT machno, start_dt, end_dt,
         CASE WHEN start_dt > MAX(end_dt) OVER (PARTITION BY machno
                                                ORDER BY start_dt
                                                ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING)
              THEN 1 ELSE 0 END AS new_grp_flag
    FROM valid
),
grouped AS (
  SELECT machno, start_dt, end_dt,
         SUM(new_grp_flag) OVER (PARTITION BY machno ORDER BY start_dt) AS grp_id
    FROM flagged
),
merged AS (
  SELECT machno, grp_id,
         MIN(start_dt) AS grp_start,
         MAX(end_dt)   AS grp_end,
         COUNT(*)      AS sessions_in_grp
    FROM grouped
   GROUP BY machno, grp_id
),
per_tt AS (
  SELECT machno,
         COUNT(*)                                                  AS interval_groups,
         SUM(sessions_in_grp)                                       AS sessions_total,
         SUM((grp_end - grp_start) * 1440)                          AS active_min_merged,
         MAX(CASE WHEN sessions_in_grp > 1 THEN 1 ELSE 0 END)       AS has_overlap
    FROM merged
   GROUP BY machno
),
stops AS (
  SELECT ws.MCH_STOP_MACHNO AS machno,
         GREATEST(TO_DATE(SUBSTR(ws.MCH_STOP_START_DT,1,14),'YYYYMMDDHH24MISS'),
                  (SELECT win_start FROM params)) AS s_dt,
         LEAST(TO_DATE(SUBSTR(ws.MCH_STOP_END_DT,1,14),'YYYYMMDDHH24MISS'),
               (SELECT win_end FROM params))     AS e_dt
    FROM TOSADM.MCH_WORKSTOP ws
    CROSS JOIN params p
   WHERE ws.MCH_STOP_STARTDATE = p.day_str OR ws.MCH_STOP_ENDDATE = p.day_str
),
stop_per_tt AS (
  SELECT machno, SUM((e_dt - s_dt) * 1440) AS stop_min_clipped
    FROM stops WHERE s_dt < e_dt GROUP BY machno
)
SELECT /*+ NO_PARALLEL */
       t.machno,
       t.sessions_total,
       t.interval_groups,
       t.has_overlap                                                       AS logout_anomaly,
       ROUND(t.active_min_merged, 1)                                        AS active_min,
       ROUND(NVL(s.stop_min_clipped, 0), 1)                                 AS stop_min,
       ROUND(t.active_min_merged - NVL(s.stop_min_clipped, 0), 1)           AS productive_min,
       LEAST(1.0, ROUND((t.active_min_merged - NVL(s.stop_min_clipped, 0)) / {{ELAPSED_DENOM}}, 4)) AS k_util_capped,
       ROUND((t.active_min_merged - NVL(s.stop_min_clipped, 0)) / {{ELAPSED_DENOM}}, 4) AS k_util_raw
  FROM per_tt t
  LEFT JOIN stop_per_tt s ON t.machno = s.machno
 ORDER BY k_util_capped DESC NULLS LAST
 FETCH FIRST 50 ROWS ONLY
