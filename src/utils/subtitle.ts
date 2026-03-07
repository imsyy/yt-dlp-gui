/** 字幕解析与双语合成工具 */

interface SubBlock {
  timing: string;
  text: string;
}

/** 解析 SRT 格式字幕为块列表 */
export const parseSrtBlocks = (content: string): SubBlock[] => {
  const blocks: SubBlock[] = [];
  const normalized = content.replace(/\r/g, "");
  const parts = normalized.split("\n\n");

  for (const part of parts) {
    const lines = part.trim().split("\n");
    if (lines.length >= 3) {
      // 第1行是序号，第2行是时间轴，剩余是文本
      blocks.push({ timing: lines[1], text: lines.slice(2).join("\n") });
    }
  }

  return blocks;
};

/** 解析 VTT 格式字幕为块列表 */
export const parseVttBlocks = (content: string): SubBlock[] => {
  const blocks: SubBlock[] = [];
  const normalized = content.replace(/\r/g, "");
  const parts = normalized.split("\n\n");

  for (const part of parts) {
    const lines = part.trim().split("\n");
    if (!lines.length) continue;

    // 找到包含 --> 的时间行
    const timingIdx = lines.findIndex((l) => l.includes("-->"));
    if (timingIdx >= 0 && timingIdx + 1 < lines.length) {
      blocks.push({
        timing: lines[timingIdx],
        text: lines.slice(timingIdx + 1).join("\n"),
      });
    }
  }

  return blocks;
};

/** 合并两份 SRT 字幕为双语字幕 */
export const mergeBilingualSrt = (primary: string, secondary: string): string => {
  const pBlocks = parseSrtBlocks(primary);
  const sBlocks = parseSrtBlocks(secondary);
  const maxLen = Math.max(pBlocks.length, sBlocks.length);

  const lines: string[] = [];
  for (let i = 0; i < maxLen; i++) {
    lines.push(String(i + 1));
    const pb = pBlocks[i];
    const sb = sBlocks[i];

    if (pb) {
      lines.push(pb.timing);
      lines.push(pb.text);
      if (sb) lines.push(sb.text);
    } else if (sb) {
      lines.push(sb.timing);
      lines.push(sb.text);
    }
    lines.push(""); // 空行分隔
  }

  return lines.join("\r\n");
};

/** 合并两份 VTT 字幕为双语字幕 */
export const mergeBilingualVtt = (primary: string, secondary: string): string => {
  const pBlocks = parseVttBlocks(primary);
  const sBlocks = parseVttBlocks(secondary);
  const maxLen = Math.max(pBlocks.length, sBlocks.length);

  const lines: string[] = ["WEBVTT", ""];
  for (let i = 0; i < maxLen; i++) {
    const pb = pBlocks[i];
    const sb = sBlocks[i];

    if (pb) {
      lines.push(pb.timing);
      lines.push(pb.text);
      if (sb) lines.push(sb.text);
    } else if (sb) {
      lines.push(sb.timing);
      lines.push(sb.text);
    }
    lines.push(""); // 空行分隔
  }

  return lines.join("\r\n");
};
