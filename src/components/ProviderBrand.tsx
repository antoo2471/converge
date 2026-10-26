import chatgptWhite from "../assets/brands/chatgpt-white.svg";
import chatgptBlack from "../assets/brands/chatgpt-black.svg";
import claude from "../assets/brands/claude.svg";
import grok from "../assets/brands/grok.svg";
import kimi from "../assets/brands/kimi.svg";
import zai from "../assets/brands/zai.svg";
import minimax from "../assets/brands/minimax.svg";
import antigravity from "../assets/brands/antigravity.svg";
import opencode from "../assets/brands/opencode.svg";
import deepseek from "../assets/brands/deepseek.svg";
const marks: Record<string, string> = {
  "claude-code": claude,
  grok,
  kimi,
  zai,
  minimax,
  antigravity,
  opencode,
  deepseek,
};
export function ProviderMark({ plugin }: { plugin: string }) {
  if (plugin === "chatgpt") return <ChatGPTMark />;
  return marks[plugin] ? (
    <img
      className="provider-monochrome-mark"
      src={marks[plugin]}
      width={24}
      height={24}
      alt=""
      aria-hidden="true"
    />
  ) : null;
}

export function ChatGPTMark() {
  return (
    <span className="provider-brand-logo" aria-hidden="true">
      <img
        className="brand-on-dark"
        src={chatgptWhite}
        alt=""
        width={24}
        height={24}
      />
      <img
        className="brand-on-light"
        src={chatgptBlack}
        alt=""
        width={24}
        height={24}
      />
    </span>
  );
}
