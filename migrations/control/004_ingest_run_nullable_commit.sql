-- Make commit_sha nullable: the trigger endpoint queues a run without knowing
-- the target commit; the ingestion worker fills in commit_sha when it starts.
ALTER TABLE ingestion_runs
    ALTER COLUMN commit_sha DROP NOT NULL;
