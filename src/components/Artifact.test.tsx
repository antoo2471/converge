// @vitest-environment jsdom
import {it,expect,vi,afterEach} from 'vitest';
import {render,screen,fireEvent,waitFor,cleanup} from '@testing-library/react';
vi.mock('../lib/native',()=>({native:true,registerArtifact:vi.fn(async(code:string)=>({path:'/artifact.html',url:'http://artifact.localhost/artifact.html'})),releaseArtifact:vi.fn(async()=>{}),exportFile:vi.fn()}));
import Artifact from './Artifact';
import {registerArtifact} from '../lib/native';
afterEach(cleanup);
it('updates streamed native artifacts, preserves local edits and re-registers refreshes in an opaque sandbox',async()=>{
 const{rerender,container}=render(<Artifact source={'<p>partial'} kind="html"/>);
 await waitFor(()=>expect(registerArtifact).toHaveBeenCalledWith('<p>partial','html',expect.any(String)));
 expect(container.querySelector('iframe')?.getAttribute('sandbox')).toBe('allow-scripts');
 rerender(<Artifact source={'<p>complete</p>'} kind="html"/>);
 await waitFor(()=>expect(registerArtifact).toHaveBeenCalledWith('<p>complete</p>','html',expect.any(String)));
 fireEvent.click(screen.getByRole('button',{name:'Code'}));const editor=screen.getByRole('textbox',{name:'Code de l’artifact'});expect((editor as HTMLTextAreaElement).value).toBe('<p>complete</p>');fireEvent.change(editor,{target:{value:'<p>edited</p>'}});
 await waitFor(()=>expect(registerArtifact).toHaveBeenCalledWith('<p>edited</p>','html',expect.any(String)));
 rerender(<Artifact source={'<p>later streamed text</p>'} kind="html"/>);expect((editor as HTMLTextAreaElement).value).toBe('<p>edited</p>');const count=vi.mocked(registerArtifact).mock.calls.length;fireEvent.click(screen.getByRole('button',{name:'Relancer'}));await waitFor(()=>expect(vi.mocked(registerArtifact).mock.calls.length).toBeGreaterThan(count));
});
