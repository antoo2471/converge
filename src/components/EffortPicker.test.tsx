// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { EffortPicker } from "./EffortPicker";
import type { ReasoningEffort } from "../core/reasoning";
afterEach(cleanup);
it("changes discrete effort levels without closing the slider",()=>{
 const changed=vi.fn();
 function Harness(){const [value,setValue]=useState<ReasoningEffort>("medium");return <EffortPicker value={value} levels={["auto","low","medium","high","xhigh","max"]} onChange={next=>{changed(next);setValue(next);}}/>;}
 render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"Effort de raisonnement : Moyen"}));
 const slider=screen.getByRole("slider",{name:"Effort de raisonnement"});
 expect(slider.getAttribute("aria-valuetext")).toBe("Moyen");
 fireEvent.change(slider,{target:{value:"4"}});expect(changed).toHaveBeenLastCalledWith("xhigh");expect(slider.getAttribute("aria-valuetext")).toBe("Très élevé");expect(screen.getByRole("dialog")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"Rétablir l’effort automatique"}));expect(changed).toHaveBeenLastCalledWith("auto");expect(slider.getAttribute("aria-valuetext")).toBe("Auto");
});
it("closes with Escape and restores focus without changing effort",()=>{
 const onChange=vi.fn();render(<EffortPicker value="low" levels={["auto","low"]} onChange={onChange}/>);
 const trigger=screen.getByRole("button",{name:"Effort de raisonnement : Faible"});fireEvent.click(trigger);fireEvent.keyDown(screen.getByRole("slider"),{key:"Escape"});expect(onChange).not.toHaveBeenCalled();expect(screen.queryByRole("dialog")).toBeNull();expect(document.activeElement).toBe(trigger);
});
