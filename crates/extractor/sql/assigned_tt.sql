-- TTs in the live work pool with a YT assignment, ANY job type (DS/LD vessel + MI/MO/LC
-- yard) and the dispatch status. Used for utilization (pure TOS, no GPS):
--   A = active/dispatched = working now (numerator)
--   A + B (blocked) + Q (queued) = deployed/tasked = denominator
-- A truck with no job is not in the pool (idle slack invisible to TOS, same as before).
-- JOBSTATUS P (planned future) excluded. COMPDATE NULL = not completed. Load: LOW.
SELECT DISTINCT JOB_ODR_YTNO AS ytno, JOB_ODR_JOBSTATUS AS jobstatus
  FROM TOSADM.JOB_ORDER_LIST
 WHERE JOB_ODR_COMPDATE IS NULL
   AND JOB_ODR_JOBSTATUS IN ('A','B','Q')
   AND JOB_ODR_YTNO IS NOT NULL
   AND TRIM(JOB_ODR_YTNO) IS NOT NULL
   AND CRE_DT >= TRUNC(SYSDATE) - 2
