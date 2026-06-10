-- All TTs with an ACTIVE (in-flight) assignment, ANY job type (DS/LD/MI/MO/LC). The DS/LD
-- work pool undercounts utilization by ignoring yard moves. JOBSTATUS A = dispatched and
-- being worked; COMPDATE NULL = not yet completed. CRE_DT bounds the scan. Load: LOW.
SELECT DISTINCT JOB_ODR_YTNO AS ytno
  FROM TOSADM.JOB_ORDER_LIST
 WHERE JOB_ODR_COMPDATE IS NULL
   AND JOB_ODR_JOBSTATUS = 'A'
   AND JOB_ODR_YTNO IS NOT NULL
   AND TRIM(JOB_ODR_YTNO) IS NOT NULL
   AND CRE_DT >= TRUNC(SYSDATE) - 2
