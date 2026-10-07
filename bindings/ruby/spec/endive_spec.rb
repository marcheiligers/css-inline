# frozen_string_literal: true

require "rspec"
require_relative "../lib/css_inline"

RSpec.describe "CSSInline's Endive backend" do
  before do
    skip "JRuby-only backend" unless RUBY_ENGINE == "jruby"
  end

  it "inlines CSS into a fragment" do
    output = CSSInline.inline_fragment("<p>Hello</p>", "p { color: red; }")

    expect(output).to eq('<p style="color: red;">Hello</p>')
  end

  it "uses options stored by CSSInliner" do
    inliner = CSSInline::CSSInliner.new(keep_style_tags: true)
    output = inliner.inline("<style>p { color: red; }</style><p>Hello</p>")

    expect(output).to include("<style>p { color: red; }</style>")
    expect(output).to include('<p style="color: red;">Hello</p>')
  end

  it "uses a build-time compiled machine when one is available" do
    skip "compiled machine is not on the classpath" unless CSSInline::Endive.const_defined?(:CssInlineModule, false)

    expect(CSSInline::Endive).to be_compiled
  end

  it "validates options when an inliner is created" do
    expect { CSSInline::CSSInliner.new(base_url: "foo") }
      .to raise_error(ArgumentError, "relative URL without a base: foo")
  end

  it "matches the native batch behavior when the inputs differ in length" do
    output = CSSInline.inline_many_fragments(
      ["<p>One</p>", "<p>Two</p>"],
      ["p { color: red; }"]
    )

    expect(output).to eq(['<p style="color: red;">One</p>'])
  end
end
