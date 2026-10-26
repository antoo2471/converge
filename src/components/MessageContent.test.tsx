// @vitest-environment jsdom
import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {MessageContent} from "./MessageContent";
afterEach(cleanup);
it("renders generated raster image data but keeps remote images and SVG data blocked",()=>{
 const data="data:image/png;base64,iVBORw0KGgo=";
 render(<MessageContent content={`![Résultat](${data})\n\n![Externe](https://example.com/image.png)\n\n![SVG](data:image/svg+xml;base64,PHN2Zy8+)`} onAction={async()=>{}} onTool={async()=>""}/>);
 expect(screen.getByRole("img",{name:"Résultat"}).getAttribute("src")).toBe(data);
 expect(screen.queryByRole("img",{name:"Externe"})).toBeNull();
 expect(screen.queryByRole("img",{name:"SVG"})).toBeNull();
});
