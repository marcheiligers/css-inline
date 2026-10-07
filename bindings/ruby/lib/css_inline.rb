# frozen_string_literal: true

if RUBY_ENGINE == "jruby"
  require_relative "css_inline/endive"
else
  begin
    require "css_inline/#{RUBY_VERSION.to_f}/css_inline"
  rescue LoadError
    require "css_inline/css_inline"
  end
end
