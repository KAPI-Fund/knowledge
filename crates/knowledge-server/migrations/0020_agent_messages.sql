-- Agent chat turns persist which agent mode produced the reply and the
-- redacted event timeline so the frontend can replay tool activity for
-- historical messages. Plain RAG messages leave both columns NULL.
ALTER TABLE conversation_messages ADD COLUMN agent_mode TEXT;
ALTER TABLE conversation_messages ADD COLUMN agent_events JSONB;
