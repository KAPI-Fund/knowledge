-- 技能 job 的执行期进度（阶段 + 最新输出行），供前端轮询展示；终态时清空。
ALTER TABLE canvas_skill_jobs ADD COLUMN progress JSONB;
