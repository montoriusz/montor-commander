'use client';
import { Checkbox, useCheckboxContext } from '@ark-ui/react/checkbox';
import { type ComponentProps, type ElementType, forwardRef, type SVGProps } from 'react';
import { createStyleContext, styled } from 'styled-system/jsx';
import { checkbox } from 'styled-system/recipes';
import type { HTMLStyledProps } from 'styled-system/types';

const { withProvider, withContext } = createStyleContext(checkbox);

export type RootProps = ComponentProps<typeof Root>;
export type HiddenInputProps = ComponentProps<typeof HiddenInput>;

export const Root = withProvider(Checkbox.Root, 'root');
export const RootProvider = withProvider(Checkbox.RootProvider, 'root');
export const Control = withContext(Checkbox.Control, 'control');
export const Group = withProvider(Checkbox.Group, 'group');
export const Label = withContext(Checkbox.Label, 'label');
export const HiddenInput = Checkbox.HiddenInput;

export {
  type CheckboxCheckedState as CheckedState,
  CheckboxGroupProvider as GroupProvider,
} from '@ark-ui/react/checkbox';

const StyledSvg = styled('svg') as ElementType<SVGProps<SVGSVGElement>, 'svg'>;

export const Indicator = forwardRef<SVGSVGElement, HTMLStyledProps<'svg'>>(
  function Indicator(props, ref) {
    const { indeterminate, checked } = useCheckboxContext();

    let icon: React.ReactNode | null = null;
    if (indeterminate) {
      icon = <path d="M5 12h14" />;
    } else if (checked) {
      icon = <path d="M20 6 9 17l-5-5" />;
    }

    return (
      <Checkbox.Indicator indeterminate={indeterminate} asChild>
        <StyledSvg
          ref={ref}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="3px"
          strokeLinecap="round"
          strokeLinejoin="round"
          {...(props as SVGProps<SVGSVGElement>)}
        >
          <title>Checkmark</title>
          {icon}
        </StyledSvg>
      </Checkbox.Indicator>
    );
  },
);
