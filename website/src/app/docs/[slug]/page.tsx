import { notFound } from "next/navigation";
import { MDXRemote } from "next-mdx-remote/rsc";
import remarkGfm from "remark-gfm";
import rehypeSlug from "rehype-slug";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypePrismPlus from "rehype-prism-plus";
import { getDoc, getAllDocSlugs } from "@/lib/docs";
import { DocPagination } from "@/components/DocPagination";

export function generateStaticParams() {
  return getAllDocSlugs().map((slug) => ({ slug }));
}

export function generateMetadata({ params }: { params: { slug: string } }) {
  try {
    const doc = getDoc(params.slug);
    return { title: `${doc.title} — Lit VCS` };
  } catch {
    return { title: "Not Found — Lit VCS" };
  }
}

export default function DocPage({ params }: { params: { slug: string } }) {
  let doc;
  try {
    doc = getDoc(params.slug);
  } catch {
    notFound();
  }

  return (
    <>
      <article className="prose max-w-none">
        <MDXRemote
          source={doc.content}
          options={{
            mdxOptions: {
              format: "md",
              remarkPlugins: [remarkGfm],
              rehypePlugins: [
                rehypeSlug,
                [rehypeAutolinkHeadings, { behavior: "wrap" }],
                rehypePrismPlus,
              ],
            },
          }}
        />
      </article>
      <DocPagination slug={params.slug} />
    </>
  );
}
