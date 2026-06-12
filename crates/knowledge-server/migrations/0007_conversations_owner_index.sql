CREATE INDEX idx_conversations_project_user
  ON conversations (project_id, user_id, updated_at DESC);
