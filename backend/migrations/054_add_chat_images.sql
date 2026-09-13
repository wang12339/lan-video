-- 聊天室图片消息：
-- msg_type: 0=文本（默认，存量行兼容）、1=图片（content 为配文，可为空；
--           image_url 指向 /media/chat/{file}）
-- image_url 随行删除由应用层负责清理物理文件（admin 删言时尽力而为）。
ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS msg_type SMALLINT NOT NULL DEFAULT 0;
ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS image_url VARCHAR(255);
